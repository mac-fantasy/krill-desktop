import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("release_version", ROOT / "scripts/release_version.py")
release_version = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release_version)


class ReleaseTests(unittest.TestCase):
    def test_tag_validation(self):
        self.assertEqual(release_version.normalize("v1.10.2", tag=True), "1.10.2")
        self.assertEqual(release_version.normalize("0.1.4"), "0.1.4")
        for value in ["1.2.3", "v1.2", "v1.2.3-beta.1", "v01.2.3", "v1.2.65536", "v1.2.٣", "v1.2.3\n", "v1.2.3; echo bad"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                release_version.normalize(value, tag=True)

    def run_installer(self, *, missing_payload=False, fail_launch=False):
        with tempfile.TemporaryDirectory(prefix="krill update '") as directory:
            root = Path(directory)
            destination = root / "Krill Desktop.app"
            staging = root / ".update"
            destination.mkdir()
            staging.mkdir()
            (destination / "version").write_text("old")
            if not missing_payload:
                (staging / "Krill Desktop.app").mkdir()
                (staging / "Krill Desktop.app/version").write_text("new")
            env = dict(os.environ, TMPDIR=str(root) + "/", LAUNCH_LOG=str(root / "launch.log"), FAIL_LAUNCH="1" if fail_launch else "0")
            result = subprocess.run([
                "bash", "-c", '''
source "$1"
launch_app() {
  printf '%s\\n' "$@" >> "$LAUNCH_LOG"
  if [[ "$FAIL_LAUNCH" == 1 && "$(cat "$1/version")" == new ]]; then return 1; fi
}
show_update_failure() { echo failure > "$TMPDIR/failure"; }
install_update "${@:2}"
''', "test", str(ROOT / "scripts/macos/install-update.sh"), "999999", str(destination), str(staging), "--demo", "space and 'quote", "$(not-executed)"
            ], env=env, capture_output=True, text=True)
            expected = "old" if missing_payload or fail_launch else "new"
            self.assertEqual((destination / "version").read_text(), expected, result.stderr)
            self.assertEqual(result.returncode, 1 if expected == "old" else 0, result.stderr)
            self.assertEqual((root / "launch.log").read_text().splitlines()[-4:], [str(destination), "--demo", "space and 'quote", "$(not-executed)"])
            if expected == "new":
                self.assertFalse(staging.exists())
            else:
                self.assertTrue((root / "failure").exists())

    def test_mac_installer_replaces_and_preserves_arguments(self):
        self.run_installer()

    def test_mac_installer_restores_when_payload_is_missing(self):
        self.run_installer(missing_payload=True)

    def test_mac_installer_restores_when_relaunch_fails(self):
        self.run_installer(fail_launch=True)


if __name__ == "__main__":
    unittest.main()
