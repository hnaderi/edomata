package edomata.backend

sealed trait FSMDefinition
object FSMDefinition {
  sealed trait HasCommand[C] extends FSMDefinition
  sealed trait HasState[S] extends FSMDefinition
  sealed trait HasRejection[R] extends FSMDefinition
  sealed trait HasInternalEvent[E] extends FSMDefinition
  sealed trait HasExternalEvent[E] extends FSMDefinition

  type CommandFor[D] = D match {
    case HasCommand[c] *: t => c
    case h *: t             => CommandFor[t]
  }
  type StateFor[D] = D match {
    case HasState[s] *: t => s
    case h *: t           => StateFor[t]
  }
  type RejectionFor[D] = D match {
    case HasRejection[r] *: t => r
    case h *: t               => RejectionFor[t]
  }
  type InternalEventFor[D] = D match {
    case HasInternalEvent[e] *: t => e
    case h *: t                   => InternalEventFor[t]
  }
  type ExternalEventFor[D] = D match {
    case HasExternalEvent[e] *: t => e
    case h *: t                   => ExternalEventFor[t]
  }
}
