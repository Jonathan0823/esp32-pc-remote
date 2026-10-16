.PHONY: ota
export OTA_IP

ota:
	@python3 scripts/ota_upload.py
