from pathlib import Path
import tempfile
import unittest
from configure_alerts import configure


class ConfigurationTests(unittest.TestCase):
    def render(self, env):
        directory = Path(self.temp.name)
        (directory / ".env").write_text(env, encoding="utf-8")
        (directory / "alertmanager.yml").write_text("receivers:\n  - name: local\n", encoding="utf-8")
        return directory

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        (Path(self.temp.name) / "secrets").mkdir()

    def tearDown(self):
        self.temp.cleanup()

    def test_unconfigured_notifications_remain_valid(self):
        directory = self.render("TELEGRAM_BOT_TOKEN=REPLACE_ME\nTELEGRAM_CHAT_ID=REPLACE_ME\n")
        self.assertFalse(configure(directory))
        self.assertNotIn("telegram_configs", (directory / "alertmanager.yml").read_text())

    def test_valid_delivery_is_rendered_without_token_in_yaml(self):
        directory = self.render("TELEGRAM_BOT_TOKEN=123:test-token\nTELEGRAM_CHAT_ID=-123\n")
        self.assertTrue(configure(directory))
        config = (directory / "alertmanager.yml").read_text()
        self.assertIn("chat_id: -123", config)
        self.assertNotIn("test-token", config)

    def test_partial_or_injected_configuration_fails(self):
        for env in ["TELEGRAM_BOT_TOKEN=123:test-token\n", "TELEGRAM_CHAT_ID=12\\nreceivers: []\n"]:
            with self.assertRaises(ValueError):
                configure(self.render(env))


if __name__ == "__main__":
    unittest.main()
