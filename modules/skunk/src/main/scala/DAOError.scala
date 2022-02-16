package edomata.backend.skunk

import skunk.data.Completion

sealed trait DAOError extends Throwable
object DAOError {
  final case class PartialInsertion(expected: Int, obtained: Int)
      extends Exception(
        s"Failed to insert! expected:$expected, inserted: $obtained"
      )
      with DAOError
  final case class PartialUpdate(expected: Int, obtained: Int)
      extends Exception(
        s"Failed to update! expected:$expected, updated: $obtained"
      )
      with DAOError
  case class NotAnInsert(response: Completion)
      extends Exception(
        s"Assertion failed! expected insert query, received $response"
      )
      with DAOError
  case class NotAnUpdate(response: Completion)
      extends Exception(
        s"Assertion failed! expected update query, received $response"
      )
      with DAOError
  final case class ExistingId(ex: Throwable)
      extends Exception("Cannot insert existing id!", ex)
      with DAOError
}
