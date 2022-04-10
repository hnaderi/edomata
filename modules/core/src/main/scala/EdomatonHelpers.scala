package edomata.core

import Domain.*

type ContextOf[D] =
  RequestContext[
    CommandFor[D],
    StateFor[D],
    MetadataFor[D]
  ]

type EdomatonOf[F[_], D, T] =
  Edomaton[F, ContextOf[D], RejectionFor[D], EventFor[D], NotificationFor[D], T]
