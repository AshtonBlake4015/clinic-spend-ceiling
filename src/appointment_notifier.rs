use clinic_spend_ceiling::{decide_notification, Appointment, InfraiClient, NotificationDecision};

fn main() {
    let appointment = Appointment {
        patient_reference: "pt-42".into(),
        start_utc: "2026-09-16T09:00:00Z".into(),
        missed_visit: true,
    };

    match decide_notification(&appointment) {
        NotificationDecision::Send(text) => println!("notification queued: {text}"),
        NotificationDecision::Hold => println!("no notification needed"),
    }

    match InfraiClient::from_environment() {
        Ok(client) => {
            println!("account and AI client share {}", client.base_url);
            report("monthly budget", client.set_monthly_ceiling(100));
            report("usage timeseries", client.usage_timeseries());
            report(
                "AI draft",
                client.draft_patient_notice("Draft a concise clinic follow-up notice."),
            );
        }
        Err(error) => eprintln!("set INFRAI_API_KEY before contacting the API: {error}"),
    }
}

fn report<T>(capability: &str, result: Result<T, impl std::fmt::Display>) {
    match result {
        Ok(_) => println!("{capability}: ok"),
        Err(error) => eprintln!("{capability}: {error}"),
    }
}
