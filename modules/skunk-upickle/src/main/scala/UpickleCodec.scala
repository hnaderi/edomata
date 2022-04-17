package edomata.backend

import cats.implicits.*
import edomata.backend.BackendCodec.Binary
import edomata.backend.BackendCodec.Json
import edomata.backend.BackendCodec.JsonB
import upickle.default.*

object UpickleCodec {
  def json[T: Reader: Writer]: Json[T] = BackendCodec
    .Json[T](write(_), s => Either.catchNonFatal(read(s)).leftMap(_.getMessage))
  def jsonb[T: Reader: Writer]: JsonB[T] = BackendCodec.JsonB[T](
    write(_),
    s => Either.catchNonFatal(read(s)).leftMap(_.getMessage)
  )
  def msgpack[T: Reader: Writer]: Binary[T] =
    BackendCodec.Binary[T](
      writeBinary(_),
      i => Either.catchNonFatal(readBinary(i)).leftMap(_.getMessage)
    )
}
