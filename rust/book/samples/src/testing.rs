//! Samples of the "Testing" section: the `edomata-testkit` helpers.

#[cfg(test)]
mod tests {
    use crate::eventsourcing::{
        Account, AccountModel, Command, Notification, Rejection, account_service,
    };
    use edomata_core::nonempty;
    use edomata_testkit::EdomatonAssertions;

    #[test]
    fn expectations_read_like_the_scala_domain_suite() {
        futures::executor::block_on(async {
            // ANCHOR: testkit
            let app = account_service();
            // command, state → expected new state and notifications (in order)
            app.expect(
                &AccountModel,
                Command::Open,
                Account::New,
                Account::Open { balance: 0 },
                [Notification::AccountOpened {
                    account_id: "sut".to_string(),
                }],
            )
            .await;
            // rejections, and no notification
            app.expect_rejection_with(
                &AccountModel,
                Command::Deposit(-1),
                Account::Open { balance: 0 },
                [Rejection::BadRequest],
            )
            .await;
            let (notifications, reasons) = app
                .expect_rejection(&AccountModel, Command::Close, Account::Open { balance: 5 })
                .await;
            assert!(notifications.is_empty());
            assert_eq!(reasons, nonempty![Rejection::NotSettled]);
            // ANCHOR_END: testkit
        });
    }
}
