package edomata.backend

import cats.implicits.*
import edomata.backend.BackendCodec.Json
import edomata.backend.BackendCodec.JsonB
import io.circe.Decoder
import io.circe.Encoder
import io.circe.parser.decode
import io.circe.syntax.*

object CirceCodec {
  def jsonb[T: Encoder: Decoder]: JsonB[T] =
    BackendCodec.JsonB(_.asJson.noSpaces, decode(_).leftMap(_.getMessage))
  def json[T: Encoder: Decoder]: Json[T] =
    BackendCodec.Json(_.asJson.noSpaces, decode(_).leftMap(_.getMessage))
}
