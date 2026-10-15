Import("env")

import csv
from pathlib import Path

project_dir = Path(env.subst("$PROJECT_DIR"))
build_dir = Path(env.subst("$BUILD_DIR"))
libs_dir = Path(env.PioPlatform().get_package_dir("framework-arduinoespressif32-libs"))
model_image = libs_dir / "esp32s3" / "esp_sr" / "srmodels.bin"
partition_csv = project_dir / "partitions.csv"

if not model_image.is_file():
    raise RuntimeError("ESP_SR model image is missing: {}".format(model_image))

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
