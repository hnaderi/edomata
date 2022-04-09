package edomata.core

import Domain.*

type ContextOf[D] =
  RequestContext.Valid[
    CommandFor[D],
    StateFor[D],
    MetadataFor[D],
    RejectionFor[D]
  ]

type EdomatonOf[F[_], D, T] =
  Edomaton[F, ContextOf[D], RejectionFor[D], EventFor[D], NotificationFor[D], T]
