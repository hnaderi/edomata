package edfsm.backend

import FSMDefinition.*

final case class RequestContext[Domain](
    aggregateId: String,
    command: DomainCommand[Domain],
    state: StateFor[Domain]
)
