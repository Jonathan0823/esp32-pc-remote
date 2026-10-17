var issued = Date.now();
vars.set("requestId", String(vars.get("requestId")).replace(/-/g, "").toLowerCase());
vars.set("issuedAtMs", issued);
vars.set("expiresAtMs", issued + 5000);
