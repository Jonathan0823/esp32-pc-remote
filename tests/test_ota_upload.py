import unittest
from pathlib import Path

from scripts.ota_upload import parse_ota_password, run_espota


class OtaUploadConfigTests(unittest.TestCase):
    def test_reads_one_valid_password_definition(self):
        password = "0123456789abcdef" * 4
        self.assertEqual(
            parse_ota_password(f'#define PC_OTA_PASSWORD "{password}" // local only\n'),
            password,
        )
        self.assertEqual(parse_ota_password(f'#define PC_OTA_PASSWORD "{password}"\r\n'), password)

    def test_reads_preprocessor_line_continuation(self):
        password = "abcdef0123456789" * 4
        config = '#define PC_OTA_PASSWORD ' + "\\" + '\r\n  "' + password + '"\r\n'
        self.assertEqual(parse_ota_password(config), password)

    def test_rejects_missing_duplicate_or_nonliteral_definition(self):
        valid = '#define PC_OTA_PASSWORD "' + "a" * 64 + '"\n'
        for config in ("", valid + valid, "#define PC_OTA_PASSWORD TOKEN\n"):
            with self.subTest(config=config[:40]):
                with self.assertRaises(ValueError):
                    parse_ota_password(config)

    def test_rejects_wrong_length_or_non_lowercase_hex(self):
        for password in ("", "a" * 63, "A" * 64, "g" * 64, "a" * 65):
            with self.subTest(length=len(password)):
                with self.assertRaises(ValueError):
                    parse_ota_password(f'#define PC_OTA_PASSWORD "{password}"\n')

    def test_uploader_runs_without_debug_argument_dump(self):
        class FakeEspota:
            args = None

            @classmethod
            def main(cls, args):
                cls.args = args
                return 0

        password = "a" * 64
        self.assertEqual(run_espota(FakeEspota, "192.168.1.2", password, Path("firmware.bin")), 0)
        self.assertEqual(
            FakeEspota.args,
            ["--ip", "192.168.1.2", "--auth", password, "--file", "firmware.bin", "--progress"],
        )
        self.assertNotIn("--debug", FakeEspota.args)


if __name__ == "__main__":
    unittest.main()
