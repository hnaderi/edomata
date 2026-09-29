//! Compiles a domain program into a concrete outcome by folding the
//! decision into the model.

use std::future::Future;

use crate::{App, Decision, DomainModel, Edomaton, NonEmpty, RequestContext};

/// Outcome of executing an [`Edomaton`] against a [`DomainModel`], produced
/// by [`DomainCompiler::execute`] and [`Edomaton::execute`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EdomatonResult<S, E, R, N> {
    /// Events were accepted and applied to the state.
    Accepted {
        /// State after applying the events.
        new_state: S,
        /// The accepted events.
        events: NonEmpty<E>,
        /// Notifications to publish.
        notifications: Vec<N>,
    },
    /// The program neither accepted nor rejected.
    Indecisive {
        /// Notifications to publish.
        notifications: Vec<N>,
    },
    /// The program rejected the command.
    Rejected {
        /// Notifications to publish.
        notifications: Vec<N>,
        /// Rejection reasons.
        reasons: NonEmpty<R>,
    },
    /// The program accepted events that the model refuses to apply: the
    /// program and the model disagree.
    Conflicted {
        /// Rejection reasons reported by the model.
        reasons: NonEmpty<R>,
    },
}

/// Runs domain programs and folds their decisions into the model.
///
/// This is what a backend does with the program's response before
/// persisting it; calling it directly is the easiest way to unit-test a
/// program without any storage.
///
/// ```
/// use edomata_core::*;
///
/// struct Account;
/// impl DomainModel for Account {
///     type State = i64; type Event = i64; type Rejection = String;
///     fn initial(&self) -> i64 { 0 }
///     fn transition(&self, e: &i64, s: i64) -> Result<i64, NonEmpty<String>> {
///         if s + e < 0 { Err(NonEmpty::new("negative balance".into())) } else { Ok(s + e) }
///     }
/// }
///
/// let dsl = Account.dsl::<i64, String>();
/// // This program does not check the balance itself: the model refuses the event.
/// let app = dsl.router(move |amount| dsl.accept(amount));
/// let run = |amount, balance| {
///     let ctx = CommandMessage::new("cmd", chrono::DateTime::UNIX_EPOCH, "acc-1", amount).build_context(balance);
///     futures::executor::block_on(DomainCompiler::execute(&Account, &app, ctx))
/// };
///
/// assert_eq!(run(5, 10), EdomatonResult::Accepted { new_state: 15, events: nonempty![5], notifications: vec![] });
/// assert_eq!(run(-50, 10), EdomatonResult::Conflicted { reasons: nonempty!["negative balance".to_string()] });
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct DomainCompiler;

impl DomainCompiler {
    /// Executes `app` with `ctx` and applies the resulting decision to the
    /// current state with `model`.
    ///
    /// The outcome is [`EdomatonResult::Rejected`] when the program rejects,
    /// [`EdomatonResult::Conflicted`] when the program accepts events the
    /// model refuses, [`EdomatonResult::Indecisive`] when it accepts nothing,
    /// and [`EdomatonResult::Accepted`] otherwise. The program output `T` is
    /// discarded.
    pub fn execute<M, C, N, T>(
        model: &M,
        app: &App<C, M::State, M::Event, M::Rejection, N, T>,
        ctx: RequestContext<C, M::State>,
    ) -> impl Future<Output = EdomatonResult<M::State, M::Event, M::Rejection, N>> + Send
    where
        M: DomainModel + Sync + ?Sized,
        M::State: Clone + Send + 'static,
        M::Event: Send + 'static,
        M::Rejection: Send + 'static,
        C: Send + 'static,
        N: Send + 'static,
        T: Send + 'static,
    {
        let state = ctx.state.clone();
        let fut = app.run(ctx);
        async move {
            let response = fut.await;
            let rejected = response.result.is_rejected();
            let notifications = response.notifications;
            match model.perform(state, response.result.void()) {
                Decision::Accepted { events, result } => EdomatonResult::Accepted {
                    new_state: result,
                    events,
                    notifications,
                },
                Decision::InDecisive(_) => EdomatonResult::Indecisive { notifications },
                Decision::Rejected(reasons) if rejected => EdomatonResult::Rejected {
                    notifications,
                    reasons,
                },
                Decision::Rejected(reasons) => EdomatonResult::Conflicted { reasons },
            }
        }
    }
}

impl<C, S, R, E, N, T> Edomaton<RequestContext<C, S>, R, E, N, T>
where
    C: Send + 'static,
    S: Clone + Send + 'static,
    R: Send + 'static,
    E: Send + 'static,
    N: Send + 'static,
    T: Send + 'static,
{
    /// Executes this program with `ctx` and folds the decision into the
    /// model. See [`DomainCompiler::execute`].
    pub fn execute<M>(
        &self,
        model: &M,
        ctx: RequestContext<C, S>,
    ) -> impl Future<Output = EdomatonResult<S, E, R, N>> + Send
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync + ?Sized,
    {
        DomainCompiler::execute(model, self, ctx)
    }
}
