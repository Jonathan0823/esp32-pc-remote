#pragma once

#include "src/services/media_control_policy.h"

bool indicator_service_start();
void indicator_set_voice_listening(bool listening);
void indicator_wake_result(const char* source, bool sent);
void indicator_media_result(MediaResult result);
