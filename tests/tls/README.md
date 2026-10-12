# Test certificates (NOT secret)

Throwaway certificates for the TLS unit tests and `test-tls.sh`. The private
keys are committed on purpose: they protect nothing and are only trusted by
the test broker. Valid for 100 years.

- `ca.crt` / `ca.key`: test CA ("OrbIQ Test CA")
- `server.crt` / `server.key`: broker certificate for `localhost` and `127.0.0.1`
- `client.crt` / `client.key`: client certificate for mutual-TLS tests
- `other-ca.crt`: an unrelated CA, for the "wrong CA must fail" test

Regenerate with the openssl commands in the commit that added this directory.
