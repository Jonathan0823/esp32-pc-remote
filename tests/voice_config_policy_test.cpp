#include <assert.h>
#include <string.h>

#include "../src/services/voice_config_policy.h"

int main() {
  char normalized[] = "Open Spotify";
  voice_phrase_normalize(normalized);
  assert(strcmp(normalized, "open spotify") == 0);

  VoiceConfigSnapshot config = {};
  config.schema = VOICE_CONFIG_SCHEMA;
  config.revision = 1;
  config.count = 2;
  strcpy(config.commands[0].phrase, "open spotify");
  strcpy(config.commands[0].actionId, "spotify");
  strcpy(config.commands[1].phrase, "close Spotify");
  strcpy(config.commands[1].actionId, "close_spotify");
  assert(voice_config_is_valid(config));

  strcpy(config.commands[1].phrase, "OPEN SPOTIFY");
  assert(!voice_config_is_valid(config));
  strcpy(config.commands[1].phrase, "sync");
  assert(!voice_config_is_valid(config));
  strcpy(config.commands[1].phrase, "close spotify");
  strcpy(config.commands[1].actionId, "spotify");
  assert(!voice_config_is_valid(config));
  strcpy(config.commands[1].actionId, "close_spotify");
  assert(voice_config_is_valid(config));

  memset(config.commands[1].phrase, 'x', sizeof(config.commands[1].phrase));
  assert(!voice_config_is_valid(config));
  config.count = VOICE_CONFIG_MAX_COMMANDS + 1;
  assert(!voice_config_is_valid(config));
  config.count = 0;
  config.revision = 0;
  assert(!voice_config_is_valid(config));
}
