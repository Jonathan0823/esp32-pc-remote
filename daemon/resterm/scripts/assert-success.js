client.test("HTTP 200 and correlated response", function () {
  var body = response.json();
  tests.assert(response.statusCode === 200, "expected HTTP 200");
  tests.assert(body && body.id === vars.get("requestId"), "response ID should match request ID");
});
