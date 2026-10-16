var issued = Date.now() - 10000;
vars.set("requestId", String(vars.get("requestId")).replace(/-/g, "").toLowerCase());
vars.set("issuedAtMs", issued);
vars.set("expiresAtMs", issued + 5000);
