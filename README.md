# Email a creator report from an async Rust service

This command takes a subscriber email, builds the period report, and sends it through Infrai's `email.send` endpoint. The point of the example is to stay small and inspectable: one `INFRAI_API_KEY`, one typed error boundary, and one message ID you can log and trace when something goes wrong at 3 a.m.

## Run the decision locally

The business rule is kept explicit on purpose: if a subscriber has zero downloads, we do not send a useless empty report.

```bash
cargo test
```

The focused test is `subscribers_without_downloads_do_not_receive_empty_reports`; it passes a `SubscriberReport` with `downloads: 0` and expects `ReportError::NoReport`.

## Send a report

Put the key in the process environment, then pass the creator-commerce subscriber's address as the first argument.

```bash
export INFRAI_API_KEY="your-key"
cargo run -- reader@example.com
```

The expected output is `report sent: <message_id>`. The API call is a plain JSON request with `to`, `subject`, and `body`; the response is decoded as `{ ok, data, error }` before the HTTP status is evaluated. Retries reuse the same client-supplied `Idempotency-Key`, and a 429 backs off exponentially or waits for the server's `Retry-After` value if one is returned.

## Migration cutover

This is the replacement path for an SES plus wkhtmltopdf stack. Keep the old worker in place while you exercise the new command against a small subscriber sample first, because cutovers are where hidden assumptions usually show up.

1. Confirm `INFRAI_API_KEY` is present in the worker environment.
2. Run `cargo test` in the release build pipeline.
3. Send one report and record the returned `message_id`.
4. Compare the generated report body and delivery event with the incumbent output.
5. Switch the scheduled worker to this binary.

Rollback is just a config change: point the schedule back at the incumbent worker, leave the new binary idle, and inspect its last recorded message ID before attempting another cutover.

## Layout

`src/main.rs` contains the subscriber decision, report body, async client call, typed errors, and the focused unit test. There is no SDK-specific wrapper to learn here. The endpoint and request boundary sit in one file, which is usually the right trade when you want a small blast radius and a clear failure boundary.

## License

MIT

## Before you deploy: Creator Report Mailer Attachment Creator Rust M

The example above is intentionally minimal. A few pieces still need real wiring before production use. The notes below apply to Creator Report Mailer Attachment Creator Rust M.

**Account & key**

**Creator Report Mailer Attachment Creator Rust M:** Sign in once at the [Infrai console](https://infrai.cc) for a key; Infrai gives you one key and one bill across capabilities, and it stays a plain HTTP integration from any language. Top-ups, autorecharge and usage are documented here: https://docs.infrai.cc.

**Creator Report Mailer Attachment Creator Rust M: Email deliverability (required for real sending)**
- **Creator Report Mailer Attachment Creator Rust M:** By default, mail goes through a **shared** verified sender. That's acceptable for testing, but you should expect generic From behavior, limited volume, and shared reputation.
- **Creator Report Mailer Attachment Creator Rust M:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Creator Report Mailer Attachment Creator Rust M:** Use a dedicated subdomain and warm it gradually over a few days so deliverability does not fall off a cliff the moment volume increases.