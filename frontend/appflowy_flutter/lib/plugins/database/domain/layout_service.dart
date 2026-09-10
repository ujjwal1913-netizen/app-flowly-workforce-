import 'package:flutter/foundation.dart';
import 'package:appflowy_backend/protobuf/flowy-database2/setting_entities.pb.dart';
import 'package:appflowy_backend/protobuf/flowy-folder/view.pb.dart';

ViewLayoutPB viewLayoutFromDatabaseLayout(DatabaseLayoutPB databaseLayout) {
  switch (databaseLayout) {
    case DatabaseLayoutPB.Board:
      return ViewLayoutPB.Board;
    case DatabaseLayoutPB.Calendar:
      return ViewLayoutPB.Calendar;
    case DatabaseLayoutPB.Grid:
      return ViewLayoutPB.Grid;
    default:
      debugPrint('Unsupported database layout: $databaseLayout, falling back to Grid');
      return ViewLayoutPB.Grid;
  }
}

DatabaseLayoutPB databaseLayoutFromViewLayout(ViewLayoutPB viewLayout) {
  switch (viewLayout) {
    case ViewLayoutPB.Board:
      return DatabaseLayoutPB.Board;
    case ViewLayoutPB.Calendar:
      return DatabaseLayoutPB.Calendar;
    case ViewLayoutPB.Grid:
      return DatabaseLayoutPB.Grid;
    default:
      debugPrint('Unsupported view layout: $viewLayout, falling back to Grid');
      return DatabaseLayoutPB.Grid;
  }
}
