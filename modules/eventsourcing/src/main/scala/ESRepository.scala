package edfsm.eventsourcing

import fs2.Stream

trait ESRepository[F[_], I, E, S] extends Repository[F, I, E, S] {
  def history(streamId: I): Stream[F, EventMessage[S]]
}
