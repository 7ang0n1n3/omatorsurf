#!/usr/bin/python3 -I
"""Fixed-command bridge. Root publishes observations; the desktop only reads.

Never load configuration/code from the plugin checkout after installation.
Never accept a binary, filename, shell command, or arbitrary backend arguments.
"""
import contextlib
import fcntl
import ipaddress
import json
import os
import signal
import stat
import subprocess
import sys
import tempfile
import time

DIRECTORY = "/run/omatorsurf-bar"
BINARY = "/usr/local/bin/omatorsurf"
ENV = {"PATH": "/usr/local/bin:/usr/bin:/bin", "LC_ALL": "C", "RUST_LOG": "warn"}
LIMIT = 65536
FLAGS = ("enabled", "tor_running", "firewall_active", "killswitch", "dns_protected", "ipv6_protected")


def trusted(info, directory=False):
    kind = stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)
    if not kind or info.st_uid != 0 or info.st_mode & 0o022:
        raise RuntimeError("Status files must be root owned and not writable by other users")


@contextlib.contextmanager
def directory(create=False):
    if create:
        try:
            os.mkdir(DIRECTORY, 0o755)
        except FileExistsError:
            pass
    fd = os.open(DIRECTORY, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        trusted(os.fstat(fd), directory=True)
        yield fd
    finally:
        os.close(fd)


def read_snapshot():
    with directory() as dfd:
        fd = os.open("status.json", os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=dfd)
        with os.fdopen(fd, "rb") as stream:
            trusted(os.fstat(stream.fileno()))
            data = stream.read(LIMIT + 1)
    if len(data) > LIMIT:
        raise RuntimeError("Status observation is too large")
    value = json.loads(data)
    if value.get("schema_version") != 1:
        raise RuntimeError("Unsupported status bridge version")
    # QML validates freshness and the typed backend fields separately.
    print(json.dumps(value, separators=(",", ":")))


def publish(dfd, value, target="status.json"):
    name = f"{target}.{os.getpid()}.tmp"
    fd = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                 0o644, dir_fd=dfd)
    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(value, stream, separators=(",", ":"))
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.rename(name, target, src_dir_fd=dfd, dst_dir_fd=dfd)
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(name, dir_fd=dfd)


def run(arguments, seconds):
    # Capture a bounded tail after execution; use disk rather than unbounded RAM.
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as diagnostics:
        proc = subprocess.Popen(arguments, stdout=output, stderr=diagnostics,
                                stdin=subprocess.DEVNULL, env=ENV, start_new_session=True)
        try:
            code = proc.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGTERM)
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait()
            raise RuntimeError("Operation exceeded its deadline; inspect live status before retrying")
        output.seek(0, os.SEEK_END)
        length = output.tell()
        output.seek(max(0, length - LIMIT))
        text = output.read().decode("utf-8", errors="replace")
        diagnostics.seek(0, os.SEEK_END)
        diagnostics.seek(max(0, diagnostics.tell() - LIMIT))
        error_text = diagnostics.read().decode("utf-8", errors="replace")
    if code != 0:
        raise RuntimeError(error_text.strip() or text.strip() or f"Command exited with status {code}")
    return text


def observe_backend():
    value = json.loads(run([BINARY, "status", "--json"], 70))
    if not isinstance(value, dict) or any(type(value.get(key)) is not bool for key in FLAGS):
        raise RuntimeError("Invalid backend status fields")
    if value.get("protection_state") not in ("protected", "degraded", "disabled"):
        raise RuntimeError("Invalid backend protection state")
    if value["enabled"] != (value["protection_state"] == "protected"):
        raise RuntimeError("Inconsistent backend protection state")
    if not isinstance(value.get("errors"), list) or any(not isinstance(x, str) for x in value["errors"]):
        raise RuntimeError("Invalid backend error details")
    ip = value.get("public_ip")
    if ip is not None:
        ipaddress.IPv4Address(ip)
    if value["enabled"] and (not all(value[key] for key in FLAGS)
                             or value["protection_state"] != "protected" or ip is None):
        raise RuntimeError("Inconsistent backend protection status")
    return value


def snapshot(action_error=""):
    started = time.time()
    status = observe_backend()
    current_ip = status["public_ip"] if status["enabled"] else None
    ip_error = ""
    if status["protection_state"] == "disabled":
        # Deliberately query the ordinary route only while protection is off.
        # Never probe around the guard via SOCKS or a Tor UID exception.
        try:
            response = json.loads(run([
                "/usr/bin/curl", "--disable", "--noproxy", "*", "--proxy", "", "--ipv4",
                "--fail", "--silent", "--show-error", "--connect-timeout", "5", "--max-time", "8",
                "--proto", "=https", "https://check.torproject.org/api/ip"], 10))
            current_ip = str(ipaddress.IPv4Address(response["IP"]))
        except (RuntimeError, ValueError, KeyError, OSError) as error:
            ip_error = str(error)[-4096:]
        # A CLI operation outside this bridge may have changed routing during I/O.
        status = observe_backend()
        if status["protection_state"] != "disabled":
            current_ip = status["public_ip"] if status["enabled"] else None
    return {"schema_version": 1, "checked_at": started, "completed_at": time.time(),
            "busy": "", "status": status, "current_ip": current_ip,
            "error": action_error, "ip_error": ip_error}


def unavailable(error, busy=""):
    return {"schema_version": 1, "checked_at": time.time(), "completed_at": time.time(),
            "busy": busy, "status": None, "current_ip": None,
            "error": str(error)[-8192:], "ip_error": ""}


def timer_minutes(value):
    if not isinstance(value, str) or not value.isascii() or not value.isdecimal():
        raise ValueError("Timer interval must be a whole number of minutes (minimum 5)")
    minutes = int(value)
    if not 5 <= minutes <= 525600:
        raise ValueError("Timer interval must be between 5 and 525600 minutes")
    return minutes


def load_timer(dfd):
    try:
        fd = os.open("timer.json", os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=dfd)
    except FileNotFoundError:
        return {"minutes": 0, "due": 0}
    with os.fdopen(fd, "r") as stream:
        trusted(os.fstat(stream.fileno()))
        value = json.loads(stream.read(LIMIT))
    if (type(value.get("minutes")) is not int
            or value["minutes"] != 0 and not 5 <= value["minutes"] <= 525600
            or type(value.get("due")) not in (int, float)
            or not 0 <= value["due"] < float("inf")):
        raise ValueError("Invalid circuit timer state")
    return value


def update_timer(dfd, timer, observation):
    # Schedule against monotonic time: wall-clock adjustments cannot accelerate it.
    if timer["minutes"]:
        protected = observation.get("status") and observation["status"]["enabled"]
        if not protected or not timer["due"]:
            timer["due"] = time.monotonic() + timer["minutes"] * 60
        elif time.monotonic() >= timer["due"]:
            # Save before attempting NEWNYM so failures cannot cause rapid retries.
            timer["due"] = time.monotonic() + timer["minutes"] * 60
            publish(dfd, timer, "timer.json")
            publish(dfd, unavailable("", busy="new-circuit"))
            error = ""
            try:
                run([BINARY, "new-circuit"], 100)
            except (RuntimeError, OSError) as failure:
                error = str(failure)[-8192:]
            observation = snapshot(error)
        publish(dfd, timer, "timer.json")
    observation["circuit_timer"] = {
        "minutes": timer["minutes"],
        "next_at": time.time() + max(0, timer["due"] - time.monotonic()) if timer["minutes"] else None,
    }
    return observation


def privileged(command, minutes=None):
    if os.geteuid() != 0:
        raise RuntimeError("Use pkexec /usr/local/libexec/omatorsurf-bar " + command)
    os.umask(0o022)
    with directory(create=True) as dfd:
        lockfd = os.open("lock", os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=dfd)
        with os.fdopen(lockfd, "r+") as lock:
            trusted(os.fstat(lock.fileno()))
            deadline = time.monotonic() + 170
            while True:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    if command == "observe":
                        return 0  # Timer skips during an authenticated action.
                    if command == "cleanup" or time.monotonic() >= deadline:
                        raise RuntimeError("Another operation is running; retry after it finishes")
                    time.sleep(0.2)
            if command == "cleanup":
                # Pending authenticated processes must not resume after removal.
                marker = os.open("disabled", os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW | os.O_CLOEXEC,
                                 0o644, dir_fd=dfd)
                try:
                    trusted(os.fstat(marker))
                finally:
                    os.close(marker)
                with contextlib.suppress(FileNotFoundError):
                    os.unlink("status.json", dir_fd=dfd)
                with contextlib.suppress(FileNotFoundError):
                    os.unlink("timer.json", dir_fd=dfd)
                return 0  # Keep the lock inode until reboot, including during updates.
            if command == "activate":
                with contextlib.suppress(FileNotFoundError):
                    os.unlink("disabled", dir_fd=dfd)
                return 0
            try:
                os.stat("disabled", dir_fd=dfd, follow_symlinks=False)
            except FileNotFoundError:
                pass
            else:
                raise RuntimeError("The bar integration was removed; reinstall it before running actions")
            timer = load_timer(dfd)
            if command == "timer":
                timer = {"minutes": minutes, "due": time.monotonic() + minutes * 60 if minutes else 0}
                publish(dfd, timer, "timer.json")
            error = ""
            if command not in ("observe", "timer"):
                publish(dfd, unavailable("", busy=command))
                try:
                    run([BINARY, command], 900)
                except (RuntimeError, OSError) as failure:
                    error = str(failure)[-8192:]
            try:
                if command in ("start", "stop", "new-circuit") and timer["minutes"]:
                    timer["due"] = time.monotonic() + timer["minutes"] * 60
                    publish(dfd, timer, "timer.json")
                publish(dfd, update_timer(dfd, timer, snapshot(error)))
            except (RuntimeError, ValueError, OSError, KeyError) as failure:
                publish(dfd, unavailable(error or failure))
                if command == "observe":
                    raise
            if error:
                raise RuntimeError(error)
    return 0


def main():
    if len(sys.argv) == 3 and sys.argv[1] == "timer":
        minutes = 0 if sys.argv[2] == "off" else timer_minutes(sys.argv[2])
        return privileged("timer", minutes)
    if len(sys.argv) != 2 or sys.argv[1] not in ("read", "observe", "start", "stop", "new-circuit", "activate", "cleanup"):
        raise RuntimeError("Usage: omatorsurf-bar read|observe|start|stop|new-circuit|activate|cleanup OR timer off|MINUTES")
    if sys.argv[1] == "read":
        read_snapshot()
        return 0
    return privileged(sys.argv[1])


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, OSError, ValueError, KeyError) as failure:
        print(str(failure), file=sys.stderr)
        sys.exit(1)
