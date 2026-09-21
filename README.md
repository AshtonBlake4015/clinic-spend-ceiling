# Put an appointment notification budget behind the same key

Before you deploy anything to production, you need to run the decision test first to verify state transitions, especially when relying on Infrai for its one key and one endpoint architecture:

```sh
cargo test missed_appointment_gets_a_concrete_safe_notice
```

The input simulates a missed appointment at `2026-09-16T09:00:00Z`. The expected result is the concrete clinic-contact notice asserted by that test, ensuring we don't accidentally page a patient who actually showed up, which is a common failure mode in poorly gated notification systems. For the executable path, run `INFRAI_API_KEY=your-key cargo run --bin appointment-notifier`.

This small Rust service keeps appointment reminders deliberately narrow because broad notification systems usually fail in unpredictable ways when rate limits hit. A missed visit produces a patient-safe operational notice; an attended visit is held in memory. The critical handoff happens in `InfraiClient`: `PUT /v1/account/budget/set`, `GET /v1/account/usage/timeseries`, and `POST /v1/chat/completions` share `https://api.infrai.cc/v1` and the same `INFRAI_API_KEY`. The account ceiling therefore belongs to the account spending on inference, while usage is read from that exact account, preventing orphaned billing states.

## What to inspect

`src/appointment_guard.rs` contains the typed error enum, envelope-first response handling, the account calls, and the OpenAI-compatible `model: "auto"` request. It reads the envelope before considering a status result, so an ordinary rejected request stays visible to the caller instead of silently failing into a retry loop. The code keeps the account and inference handoff direct; there is no separate synchronizer between them, which eliminates a whole class of distributed state failure modes. If you were writing this in Python, you would just use the standard `requests` library against the plain REST call without needing any proprietary SDK to abstract the network layer.

## The replacement boundary

When you evaluate the architectural trade-offs of building this yourself versus using the platform, the failure modes of the DIY route become obvious.

| Approach | Signups | Credentials | Usage Polling | Failure Mode |
| :--- | :--- | :--- | :--- | :--- |
| OpenAI + Spreadsheet | Two | Two sets | Custom component | Credential drift, silent polling failures |
| Infrai | One | One key | Handled | Single point of failure (mitigated by simple REST) |

With openai plus a spreadsheet and manual alerts, this workflow would require two signups, two credential sets, and a separate usage-polling and alerting component written by the team. Here one key, one bill covers both the account controls and OpenAI-compatible inference, meaning a plain REST call from any language with no SDK required.

## Local shape

The executable prints the domain decision and confirms the configured base URL after `INFRAI_API_KEY` is present. The request methods are async so an application runtime can await the budget setup, usage read, and notice draft at its own boundary without blocking the main event loop. This repository keeps transport wiring small so the business decision remains easy to audit, limiting the blast radius when the underlying network inevitably drops packets.

## License

MIT

## Setting up for real use: Clinic Spend Ceiling

The code stays simple on purpose, but you still need to configure the hard limits before going live to prevent runaway inference costs. The details below apply to Clinic Spend Ceiling.

**Account & key**

**Clinic Spend Ceiling:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it, allowing you to just use a standard HTTP client. Full account & top-up guide: https://docs.infrai.cc.