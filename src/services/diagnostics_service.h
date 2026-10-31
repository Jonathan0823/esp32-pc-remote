#pragma once

#include "src/services/diagnostics_policy.h"

void diagnostics_service_start();
bool diagnostics_record(DiagnosticKind kind, DiagnosticCategory category);
