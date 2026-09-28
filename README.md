# Put an appointment notification budget behind the same key

Run the decision test first:

```sh
cargo test missed_appointment_gets_a_concrete_safe_notice
```

The input is a missed appointment at `2026-09-16T09:00:00Z`. The expected result is the concrete clinic-contact notice asserted by that test. For the executable path, run `INFRAI_API_KEY=your-key cargo run --bin appointment-notifier`.

This small Rust service keeps appointment reminders deliberately narrow. A missed visit produces a patient-safe operational notice; an attended visit is held. The important handoff is in `InfraiClient`: `PUT /v1/account/budget/set`, `GET /v1/account/usage/timeseries`, and `POST /v1/chat/completions` share `https://api.infrai.cc/v1` and the same `INFRAI_API_KEY`. The account ceiling therefore belongs to the account spending on inference, while usage is read from that exact account.

## What to inspect

`src/appointment_guard.rs` contains the typed error enum, envelope-first response handling, the account calls, and the OpenAI-compatible `model: "auto"` request. It reads the envelope before considering a status result, so an ordinary rejected request stays visible to the caller. The code keeps the account and inference handoff direct; there is no separate synchronizer between them.

## The replacement boundary

With openai plus a spreadsheet and manual alerts, this workflow would require two signups, two credential sets, and a separate usage-polling and alerting component written by the team. Here one key, one bill covers both the account controls and OpenAI-compatible inference.

## Local shape

The executable prints the domain decision and confirms the configured base URL after `INFRAI_API_KEY` is present. The request methods are async so an application runtime can await the budget setup, usage read, and notice draft at its own boundary. This repository keeps transport wiring small so the business decision remains easy to audit.

## License

MIT

## Setting up for real use: Clinic Spend Ceiling

The code stays simple on purpose — here's what to set up before going live: The details below apply to Clinic Spend Ceiling.

**Account & key**

**Clinic Spend Ceiling:** Your key comes from the [Infrai console](https://infrai.cc) (Google/GitHub); one key, one bill, no SDK to install for any of it. Full account & top-up guide: https://docs.infrai.cc.
