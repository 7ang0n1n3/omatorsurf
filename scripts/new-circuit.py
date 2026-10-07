"""Invoked by Rust with isolated Python; no shell and no secrets in output."""
import sys
import time

try:
    from stem import Signal
    from stem.control import Controller
except ImportError:
    sys.exit("Stem is missing. Install: pkexec /usr/bin/pacman -S python-stem")

try:
    with Controller.from_port(address="127.0.0.1", port=int(sys.argv[1])) as controller:
        expected = "/var/lib/omatorsurf/tor/control_auth_cookie"
        if controller.get_protocolinfo().cookie_path != expected:
            sys.exit("Control endpoint advertised an unexpected cookie file")
        controller.authenticate()
        if controller.get_conf("CookieAuthFile") != expected:
            sys.exit("Control endpoint is not using the application's cookie file")
        time.sleep(controller.get_newnym_wait())
        controller.signal(Signal.NEWNYM)
except Exception as error:
    sys.exit("Tor circuit request failed: " + type(error).__name__)
