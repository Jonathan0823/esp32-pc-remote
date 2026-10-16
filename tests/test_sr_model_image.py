import csv
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MODEL_IMAGE = ROOT / "models" / "srmodels.bin"


class SpeechModelImageTest(unittest.TestCase):
    def test_image_contains_jarvis_and_existing_speech_models(self):
        image = MODEL_IMAGE.read_bytes()

        for model in (b"wn9_jarvis_tts", b"mn7_en", b"vadnet1_medium"):
            with self.subTest(model=model):
                self.assertIn(model, image)
        self.assertNotIn(b"wn9_hiesp", image)

    def test_image_fits_the_model_partition(self):
        with (ROOT / "partitions.csv").open(newline="") as source:
            partition = next(row for row in csv.reader(source) if row and row[0].strip() == "model")

        self.assertLessEqual(MODEL_IMAGE.stat().st_size, int(partition[4].strip(), 0))


if __name__ == "__main__":
    unittest.main()
