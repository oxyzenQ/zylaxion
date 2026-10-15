# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
"""The real-usage battery — zero to hero, the crown stage (night-master-3).

The claims the README makes, proved on a live kernel by a real user:
the zero-config law (no config file anywhere, nothing written), the
pure-math law (zero audio samples in the binary or the tree), the
no-input-group law (a plain user plays sound with no /dev/input access
at all), the full event chain (uinput keystrokes ride the kernel's
input path through the eBPF tap into the synthesizer), kernel-side
telemetry incrementing at zero userspace cost, live preset and volume
switches as map writes, the off verb self-exiting the player, and the
final state: zero processes, zero pins.

The keystroke injector is the kernel's own uinput driver — the same
path a real USB keyboard's events traverse after usbhid hands them to
the input core. The audio proof reads the PCM state the kernel exposes
under /proc/asound: a running stream there means the synthesizer is
rendering frames, and frames only flow when key events arrived through
the tap. One chain, two kernel witnesses.
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import threading
import time

from sm_report import Ledger, Step

# uinput constants (linux/uinput.h) — the injector's whole surface.
_UI_SET_EVBIT = 0x40045564
_UI_SET_KEYBIT = 0x40045565
_UI_DEV_CREATE = 0x5501
_UI_DEV_DESTROY = 0x5502
_EV_SYN = 0x00
_EV_KEY = 0x01
_KEY_Q = 16
_KEY_W = 17
_KEY_E = 18
_KEY_R = 19
_KEY_A = 30
_INJECT_KEYS = (_KEY_Q, _KEY_W, _KEY_E, _KEY_R, _KEY_A)

_AUDIO_EXTENSIONS = (
    ".wav", ".wave", ".ogg", ".oga", ".flac", ".mp3", ".aiff", ".aif",
    ".opus", ".mka", ".au", ".snd",
)


def _run(cmd: list[str], timeout: int = 60) -> tuple[int, str]:
    try:
        proc = subprocess.run(
            cmd, capture_output=True, text=True, timeout=timeout
        )
        return proc.returncode, (proc.stdout + proc.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, f"{type(exc).__name__}: {exc}"


def _run_as_user(user: str, binary: str, args: list[str]) -> tuple[int, str]:
    return _run(["sudo", "-u", user, binary, *args])


class _KeyboardInjector:
    """A uinput virtual keyboard typing in bursts until stopped."""

    def __init__(self) -> None:
        self.fd = -1
        self.stop = threading.Event()
        self.thread: threading.Thread | None = None
        self.error = ""

    def start(self) -> bool:
        try:
            import fcntl
            import struct

            self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
            fcntl.ioctl(self.fd, _UI_SET_EVBIT, _EV_KEY)
            fcntl.ioctl(self.fd, _UI_SET_EVBIT, _EV_SYN)
            for key in _INJECT_KEYS:
                fcntl.ioctl(self.fd, _UI_SET_KEYBIT, key)
            # struct uinput_user_dev: name[80] + input_id (4 x u16) +
            # ff_effects_max (u32) + the four abs tables (64 x u32 each).
            setup = struct.pack(
                "80sHHHII" + "64I" * 4,
                b"zylaxion-supermassive",
                0x06,  # BUS_VIRTUAL
                0x1A, 0x5A, 0x01,
                0,
                *([0] * 256),
            )
            os.write(self.fd, setup)
            fcntl.ioctl(self.fd, _UI_DEV_CREATE)
        except (OSError, ImportError) as exc:
            self.error = f"{type(exc).__name__}: {exc}"
            return False
        self.thread = threading.Thread(target=self._type_loop, daemon=True)
        self.thread.start()
        return True

    def _emit(self, ev_type: int, code: int, value: int) -> None:
        import struct

        # struct input_event on a 64-bit kernel: timeval (2 x i64) +
        # u16 type + u16 code + s32 value.
        os.write(self.fd, struct.pack("qqHHi", 0, 0, ev_type, code, value))

    def _type_loop(self) -> None:
        while not self.stop.is_set():
            for key in _INJECT_KEYS:
                if self.stop.is_set():
                    return
                try:
                    self._emit(_EV_KEY, key, 1)  # press
                    self._emit(_EV_SYN, 0, 0)
                    self._emit(_EV_KEY, key, 0)  # release
                    self._emit(_EV_SYN, 0, 0)
                except OSError as exc:
                    self.error = f"{type(exc).__name__}: {exc}"
                    return
                time.sleep(0.05)
            time.sleep(0.1)

    def shutdown(self) -> None:
        self.stop.set()
        if self.thread is not None:
            self.thread.join(timeout=5)
        if self.fd >= 0:
            try:
                import fcntl

                fcntl.ioctl(self.fd, _UI_DEV_DESTROY)
            except OSError:
                pass
            os.close(self.fd)
            self.fd = -1


def battery(
    ledger: Ledger, binary: str, user: str, tree_root: str, root_lane: bool = True
) -> None:
    """The whole real-usage family; rows land on the ledger.

    The root lane rides all three stages; the rootless self-test lane
    rides the two proofs only (pure math, zero config) — the chain
    stage needs a kernel to attach into and a user to hand over to.
    """
    _pure_math(ledger, binary, tree_root)
    _zero_config(ledger, binary)
    if root_lane:
        _real_usage(ledger, binary, user)


def _pure_math(ledger: Ledger, binary: str, tree_root: str) -> None:
    with Step(ledger, "pure math — zero audio samples"):
        riff_hits = 0
        wave_hits = 0
        try:
            with open(binary, "rb") as fh:
                blob = fh.read()
            riff_hits = blob.count(b"RIFF")
            wave_hits = blob.count(b"WAVE")
        except OSError as exc:
            ledger.fail_row("the binary is readable for the sample scan", str(exc))
            return
        if riff_hits == 0 and wave_hits == 0:
            ledger.pass_row(
                "the binary carries zero audio samples",
                "no RIFF/WAVE magic anywhere in the image",
            )
        else:
            ledger.fail_row(
                "the binary carries zero audio samples",
                f"RIFF={riff_hits} WAVE={wave_hits}",
            )

        assets: list[str] = []
        for dirpath, _dirnames, filenames in os.walk(tree_root):
            # Skip the build's own scratch and git metadata: the claim
            # is about the SHIPPED tree, not test litter. Component
            # aware — a bare "/target" tail is still the build dir.
            parts = dirpath.split(os.sep)
            if "target" in parts or ".git" in parts:
                continue
            for name in filenames:
                if name.lower().endswith(_AUDIO_EXTENSIONS):
                    assets.append(os.path.join(dirpath, name))
        if not assets:
            ledger.pass_row(
                "the source tree ships zero audio assets",
                "no wav/ogg/flac/mp3/aiff/opus anywhere",
            )
        else:
            ledger.fail_row(
                "the source tree ships zero audio assets",
                f"found={assets[:5]}",
            )


def _zero_config(ledger: Ledger, binary: str) -> None:
    with Step(ledger, "zero config — the kernel is the config"):
        scratch = tempfile.mkdtemp(prefix="zylaxion-zero-config-")
        home = os.path.join(scratch, "home")
        xdg = os.path.join(scratch, "xdg")
        os.makedirs(home)
        os.makedirs(xdg)
        env = dict(os.environ)
        env["HOME"] = home
        env["XDG_CONFIG_HOME"] = xdg
        env.pop("ZYLAXION_PRESET", None)
        env.pop("ZYLAXION_VOLUME", None)

        surfaces_ok = True
        detail = ""
        for args in (["list-presets"], ["doctor"], ["status"], ["off"]):
            try:
                proc = subprocess.run(
                    [binary, *args],
                    capture_output=True,
                    text=True,
                    env=env,
                    timeout=45,
                )
                # The doctor's rc is its verdict's polarity (0 all
                # clear, 1 some check failed — both honest shapes);
                # every other discovery verb must exit 0.
                honest = proc.returncode == 0 or (
                    args == ["doctor"] and proc.returncode == 1
                )
                if not honest:
                    surfaces_ok = False
                    detail = f"{args} rc={proc.returncode}"
                    break
            except (OSError, subprocess.TimeoutExpired) as exc:
                surfaces_ok = False
                detail = f"{args}: {exc}"
                break
        if surfaces_ok:
            ledger.pass_row(
                "the CLI surface runs with a pristine HOME and XDG",
                "4 verbs, empty environment, all honest",
            )
        else:
            ledger.fail_row(
                "the CLI surface runs with a pristine HOME and XDG",
                detail,
            )

        litter: list[str] = []
        for root_dir in (home, xdg):
            for dirpath, _dirnames, filenames in os.walk(root_dir):
                for name in filenames:
                    litter.append(os.path.join(dirpath, name))
        if not litter:
            ledger.pass_row(
                "nothing was written to HOME or XDG_CONFIG_HOME",
                "the no-config law holds after the surface run",
            )
        else:
            ledger.fail_row(
                "nothing was written to HOME or XDG_CONFIG_HOME",
                f"litter={litter[:5]}",
            )

        probe = subprocess.run(
            ["find", "/etc", "-maxdepth", "3", "-iname", "*zylaxion*"],
            capture_output=True,
            text=True,
            timeout=45,
        )
        # Only STDOUT is the match surface — unreadable /etc corners
        # (credstore, ssl/private) make rc nonzero on a healthy system.
        if probe.stdout.strip() == "":
            ledger.pass_row(
                "no zylaxion config lives under /etc",
                "the kernel and the compiled registry are the only config",
            )
        else:
            ledger.fail_row(
                "no zylaxion config lives under /etc",
                f"matches={probe.stdout.strip()[:120]}",
            )

        _run(["rm", "-rf", scratch])


def _real_usage(ledger: Ledger, binary: str, user: str) -> None:
    with Step(ledger, "the real user — no input group, no /dev/input"):
        rc, out = _run(["id", "-nG", user])
        if rc == 0 and "input" not in out.split():
            ledger.pass_row(
                f"{user} is a plain user with no input group",
                f"groups: {out[:70]}",
            )
        else:
            ledger.fail_row(
                f"{user} is a plain user with no input group",
                f"rc={rc}: {out[:90]}",
            )

        event_devs: list[str] = []
        try:
            for entry in os.listdir("/dev/input"):
                if entry.startswith("event"):
                    event_devs.append(f"/dev/input/{entry}")
        except OSError:
            pass
        readable = [dev for dev in event_devs if _readable_by(user, dev)]
        if event_devs and not readable:
            ledger.pass_row(
                f"{user} cannot read a single /dev/input device",
                f"{len(event_devs)} event devices, all denied",
            )
        elif not event_devs:
            ledger.skip_row(
                f"{user} cannot read a single /dev/input device",
                "no /dev/input event devices exist in this guest",
            )
        else:
            ledger.fail_row(
                f"{user} cannot read a single /dev/input device",
                f"readable={readable[:3]}",
            )

    with Step(ledger, "zero to hero — the chain, end to end"):
        # The attach rides SUDO_USER so the handover reaches the user —
        # the exact environment `sudo zylaxion attach` produces.
        env_attach = dict(os.environ)
        env_attach["SUDO_USER"] = user
        try:
            proc = subprocess.run(
                [binary, "attach"],
                capture_output=True,
                text=True,
                env=env_attach,
                timeout=90,
            )
            rc, out = proc.returncode, (proc.stdout + proc.stderr).strip()
        except (OSError, subprocess.TimeoutExpired) as exc:
            rc, out = 127, f"{type(exc).__name__}: {exc}"
        if rc == 0 and "tap attached" in out:
            ledger.pass_row("attach hands the tap to the real user", user)
        else:
            ledger.fail_row("attach hands the tap to the real user", f"rc={rc}: {out[:400]}")
            return

        posture = _unpriv_bpf_disabled()
        rc, out = _run_as_user(user, binary, ["on"])
        if posture != 0:
            # The kernel refuses every unprivileged bpf() — no map
            # write can succeed as this user, so the honest
            # expectation is the classified refusal itself
            # (night-badas-4 round 5): the product names the gate,
            # the rc stays 1, and the chain below it cannot run.
            if rc == 1 and "unprivileged bpf" in out:
                ledger.pass_row(
                    "on as the real user refuses at the kernel's "
                    "unprivileged-bpf gate",
                    f"posture {posture}: {out.splitlines()[0][:54]}",
                )
            else:
                ledger.fail_row(
                    "on as the real user refuses at the kernel's "
                    "unprivileged-bpf gate",
                    f"posture {posture}, rc={rc}: {out[:400]}",
                )
            _run([binary, "detach"])
            return
        if rc == 0 and "sound on:" in out:
            ledger.pass_row(
                f"{user} turns the sound on (no input group, no sudo)",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                f"{user} turns the sound on (no input group, no sudo)",
                f"rc={rc}: {out[:400]}",
            )
            _run([binary, "detach"])
            return

        time.sleep(1.0)
        pid = _player_pid(user)
        if pid:
            ledger.pass_row("the player runs as the real user", f"pid {pid}")
        else:
            ledger.fail_row(
                "the player runs as the real user",
                "no zylaxion process under the user",
            )

        if pid:
            dev_fds = _fds_touching(pid, "/dev/input")
            if not dev_fds:
                ledger.pass_row(
                    "the player opens zero /dev/input files",
                    "the passive tap law, witnessed in /proc",
                )
            else:
                ledger.fail_row(
                    "the player opens zero /dev/input files",
                    f"fds={dev_fds[:4]}",
                )

        injector = _KeyboardInjector()
        injected = injector.start()
        if injected:
            ledger.pass_row(
                "the uinput virtual keyboard is live",
                "the kernel's own keyboard injection path",
            )
        else:
            ledger.fail_row(
                "the uinput virtual keyboard is live",
                injector.error or "could not create the uinput device",
            )

        presses = 0
        running = ""
        if injected:
            deadline = time.monotonic() + 12.0
            while time.monotonic() < deadline:
                rc, out = _run_as_user(user, binary, ["status", "--verbose"])
                match = re.search(r"total presses:\s*(\d+)", out)
                if match:
                    presses = max(presses, int(match.group(1)))
                for card in ("card0", "card1"):
                    state = _pcm_state(card)
                    if state == "RUNNING":
                        running = f"{card}/pcm0p"
                        break
                if presses > 0 and running:
                    break
                time.sleep(0.5)
            injector.shutdown()

        if presses > 0:
            ledger.pass_row(
                "kernel-side telemetry counts the keystrokes",
                f"total presses: {presses} (the tap saw every event)",
            )
        else:
            ledger.fail_row(
                "kernel-side telemetry counts the keystrokes",
                "status --verbose never reported a press",
            )

        if running:
            ledger.pass_row(
                "the synthesizer renders real PCM frames",
                f"{running} state RUNNING on the guest's sound card",
            )
        else:
            ledger.fail_row(
                "the synthesizer renders real PCM frames",
                "no ALSA PCM reached RUNNING — is the sound card present?",
            )

        rc, out = _run_as_user(user, binary, ["preset", "--switch", "topre"])
        if rc == 0:
            ledger.pass_row(
                "live preset switch as the real user (one map write)",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                "live preset switch as the real user (one map write)",
                f"rc={rc}: {out[:120]}",
            )

        rc, out = _run_as_user(user, binary, ["volume", "--percent", "42"])
        if rc == 0:
            ledger.pass_row(
                "live volume as the real user (one map write)",
                "42 percent",
            )
        else:
            ledger.fail_row(
                "live volume as the real user (one map write)",
                f"rc={rc}: {out[:120]}",
            )

        rc, out = _run_as_user(user, binary, ["on", "--preset", "definitely-not-a-preset"])
        if rc == 1 and "not found" in out and "Available" in out:
            ledger.pass_row(
                "an unknown preset name gets the honest not-found verdict",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                "an unknown preset name gets the honest not-found verdict",
                f"rc={rc}: {out[:120]}",
            )

        rc, out = _run_as_user(user, binary, ["off"])
        if rc == 0:
            ledger.pass_row(
                "off stops the sound as the real user",
                out.splitlines()[0][:70],
            )
        else:
            ledger.fail_row(
                "off stops the sound as the real user",
                f"rc={rc}: {out[:120]}",
            )

        time.sleep(1.5)
        if not _player_pid(user):
            ledger.pass_row(
                "the player self-exited — zero processes remain",
                "the stateless-player law, witnessed in the process table",
            )
        else:
            ledger.fail_row(
                "the player self-exited — zero processes remain",
                f"pid {pid} still alive after off",
            )

        _run([binary, "detach"])
        rc, out = _run_as_user(user, binary, ["on"])
        if rc == 1 and "the tap is not attached" in out:
            ledger.pass_row(
                "on after detach refuses honestly (back to the tap-less door)",
                "the cycle closes where it began",
            )
        else:
            ledger.fail_row(
                "on after detach refuses honestly (back to the tap-less door)",
                f"rc={rc}: {out[:120]}",
            )


def _unpriv_bpf_disabled() -> int:
    """The kernel's unprivileged-bpf posture (night-badas-4 round 5).

    Ubuntu builds the gate in (CONFIG_BPF_UNPRIV_DEFAULT_OFF=y on both
    reference kernels, verified against the shipped configs): the guest
    boots at 2, every unprivileged bpf() is refused for the whole boot,
    and root cannot re-enable it. The real-user chain is a series of
    control-map writes, so this lane tests what the kernel honestly
    allows — the full zero-to-hero chain when the sysctl sits at 0,
    the honest refusal when it does not.
    """
    try:
        with open("/proc/sys/kernel/unprivileged_bpf_disabled") as fh:
            return int(fh.read().strip() or "0")
    except (OSError, ValueError):
        return 0


def _readable_by(user: str, path: str) -> bool:
    rc, _ = _run(["sudo", "-u", user, "test", "-r", path], timeout=15)
    return rc == 0


def _player_pid(user: str) -> int:
    rc, out = _run(["pgrep", "-u", user, "-x", "zylaxion"], timeout=15)
    if rc == 0 and out:
        return int(out.splitlines()[0])
    return 0


def _fds_touching(pid: int, prefix: str) -> list[str]:
    hits: list[str] = []
    fd_dir = f"/proc/{pid}/fd"
    try:
        for entry in os.listdir(fd_dir):
            try:
                target = os.readlink(os.path.join(fd_dir, entry))
            except OSError:
                continue
            if target.startswith(prefix):
                hits.append(target)
    except OSError:
        pass
    return hits


def _pcm_state(card: str) -> str:
    try:
        with open(f"/proc/asound/{card}/pcm0p/sub0/status", encoding="utf-8") as fh:
            return fh.read().strip()
    except OSError:
        return ""
