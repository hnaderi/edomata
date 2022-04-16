package edomata.backend

import skunk.Codec
import skunk.codec.binary.bytea
import skunk.data.Type

import scala.annotation.implicitNotFound

@implicitNotFound(
  "Cannot find how to handle serialization/deserialization for ${T}"
)
sealed trait BackendCodec[T] {
  def oid: Type
  def codec: Codec[T]
}

object BackendCodec {
  @implicitNotFound("Cannot find a way to build json codec for ${T}")
  final class Json[T](encode: T => String, decode: String => Either[String, T])
      extends BackendCodec[T] {
    final def oid = Type.json
    def codec: Codec[T] = Codec.simple(encode, decode, oid)
  }

  @implicitNotFound("Cannot find a way to build jsonb codec for ${T}")
  final class JsonB[T](encode: T => String, decode: String => Either[String, T])
      extends BackendCodec[T] {
    final def oid = Type.jsonb
    def codec: Codec[T] = Codec.simple(encode, decode, oid)
  }

  @implicitNotFound("Cannot find a way to build binary codec for ${T}")
  final class Binary[T](
      encode: T => Array[Byte],
      decode: Array[Byte] => Either[String, T]
  ) extends BackendCodec[T] {
    def oid: Type = Type.bytea
    def codec: Codec[T] = bytea.eimap(decode)(encode)
  }
}
