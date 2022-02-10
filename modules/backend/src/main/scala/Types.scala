package edfsm.backend

import cats.Applicative
import cats.data.EitherNec
import cats.data.Kleisli
import cats.data.ValidatedNec
import edfsm.core.Action
import edfsm.core.Decision
import edfsm.core.DecisionT
import edfsm.eventsourcing.Fold
import edfsm.protocols.command.CommandMessage

import FSMDefinition.*

type DomainDecider[F[_], Domain] =
  DecisionT[F, RejectionFor[Domain], InternalEventFor[Domain], Unit]

type DomainDecision[Domain, T] =
  Decision[RejectionFor[Domain], InternalEventFor[Domain], T]

type DomainTransition[Domain] =
  Fold[StateFor[Domain], InternalEventFor[Domain], RejectionFor[Domain]]

type DomainCommand[Domain] = CommandMessage[CommandFor[Domain]]

type CommandHandler[F[_], Domain] =
  DomainCommand[Domain] => F[CommandResult[Domain]]
type DomainAction[F[_], Domain] = F[CommandResult[Domain]]
type CommandResult[Domain] = EitherNec[RejectionFor[Domain], Unit]

type DomainLogic[F[_], Domain] =
  RequestContext[Domain] => DomainAR[F, Domain, Unit]

type DomainAR[F[_], Domain, T] =
  Action[F, RejectionFor[Domain], InternalEventFor[Domain], ExternalEventFor[
    Domain
  ], T]
