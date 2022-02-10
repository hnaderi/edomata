package edfsm.core

final case class Request[+C, +S](command: C, state: Option[S])
