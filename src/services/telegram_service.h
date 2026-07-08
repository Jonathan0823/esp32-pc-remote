#ifndef TELEGRAM_SERVICE_H
#define TELEGRAM_SERVICE_H

#include <Arduino.h>
#include <ArduinoJson.h>

void telegram_setup();
void telegram_poll();

bool telegram_send_json_once(const String& command, JsonObject payload, const char* label);
bool telegram_send_text_once(const String& chatId, const String& text, const String& parse_mode = "");
bool telegram_send_callback_answer_once(const String& query_id,
                                        const String& text = "",
                                        bool show_alert = false,
                                        const String& url = "",
                                        int cache_time = 0);

#endif
