#include <assert.h>
#include "../src/services/diagnostics_policy.h"

int main() {
  assert(diagnostics_retry_delay_ms(0) == 1000);
  assert(diagnostics_retry_delay_ms(1) == 2000);
  assert(diagnostics_retry_delay_ms(5) == 30000);
  assert(diagnostics_retry_delay_ms(20) == 30000);
  DiagnosticRing ring;
  for (uint32_t i = 0; i < DIAGNOSTICS_RING_CAPACITY + 3; ++i) {
    ring.push({i, 0, false, DiagnosticKind::ClockState, DiagnosticCategory::NotReady});
  }
  assert(ring.size() == DIAGNOSTICS_RING_CAPACITY);
  assert(ring.dropped() == 3);
  DiagnosticRecord batch[DIAGNOSTICS_BATCH_CAPACITY];
  assert(ring.copy(batch, DIAGNOSTICS_BATCH_CAPACITY) == DIAGNOSTICS_BATCH_CAPACITY);
  assert(batch[0].uptimeMs == 3);
  ring.discard(DIAGNOSTICS_BATCH_CAPACITY);
  assert(ring.size() == DIAGNOSTICS_RING_CAPACITY - DIAGNOSTICS_BATCH_CAPACITY);
  assert(ring.copy(batch, DIAGNOSTICS_BATCH_CAPACITY) == DIAGNOSTICS_BATCH_CAPACITY);
  assert(batch[0].uptimeMs == 11);
}
