#include <assert.h>
#include "../src/services/media_control_config.h"

int main() {
  assert(media_control_config_status() == MediaConfigStatus::Ready);
  return 0;
}
