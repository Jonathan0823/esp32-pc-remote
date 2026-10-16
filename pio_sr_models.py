Import("env")

import csv
from pathlib import Path

project_dir = Path(env.subst("$PROJECT_DIR"))
model_image = project_dir / "models" / "srmodels.bin"
partition_csv = project_dir / "partitions.csv"

if not model_image.is_file():
    raise RuntimeError("ESP_SR model image is missing: {}".format(model_image))

model_data = model_image.read_bytes()
for required_model in (b"wn9_jarvis_tts", b"mn7_en", b"vadnet1_medium"):
    if required_model not in model_data:
        raise RuntimeError("ESP_SR model image is missing {}".format(required_model.decode()))
if b"wn9_hiesp" in model_data:
    raise RuntimeError("ESP_SR model image still contains the Hi ESP wake word")

with partition_csv.open(newline="") as partitions_file:
    model_partition = next(
        (row for row in csv.reader(partitions_file) if row and row[0].strip() == "model"),
        None,
    )

if model_partition is None or len(model_partition) < 5:
    raise RuntimeError("partitions.csv must define a model partition")

offset = int(model_partition[3].strip(), 0)
partition_size = int(model_partition[4].strip(), 0)
model_size = model_image.stat().st_size
if offset + partition_size > 16 * 1024 * 1024:
    raise RuntimeError("model partition exceeds the configured 16 MB flash")
if model_size > partition_size:
    raise RuntimeError(
        "ESP_SR model image ({} bytes) exceeds model partition ({} bytes)".format(model_size, partition_size)
    )

# PIO does not apply Arduino boards.txt upload.extra_flags, so attach the packaged model explicitly.
env.Append(FLASH_EXTRA_IMAGES=[(hex(offset), str(model_image))])
