#pragma once

#include "src/services/voice_config_policy.h"

bool voice_service_start();
bool voice_service_apply_config(const VoiceConfigSnapshot& config);
