#pragma once

bool indicator_service_start();
void indicator_set_voice_listening(bool listening);
void indicator_wake_result(const char* source, bool sent);
