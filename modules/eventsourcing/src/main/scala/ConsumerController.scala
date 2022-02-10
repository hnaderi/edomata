package edfsm.eventsourcing

import fs2.Stream

type ConsumerName = String

trait ConsumerController[F[_]] {
  def seek(name: ConsumerName, seqNr: SeqNr): F[Unit]
  def read(name: ConsumerName): F[Option[SeqNr]]
}
