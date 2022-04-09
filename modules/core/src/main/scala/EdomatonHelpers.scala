package edomata.core

import Domain.*

type CTX[D] =
  RequestContext.Valid[CommandFor[D], StateFor[D], MetadataFor[
    D
  ], RejectionFor[D]]

type SM[F[_], D, T] =
  Edomaton[F, CTX[D], RejectionFor[D], EventFor[D], NotificationFor[D], T]
