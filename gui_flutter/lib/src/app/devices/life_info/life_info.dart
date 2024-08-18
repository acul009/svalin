import 'package:flutter/widgets.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:gui_flutter/src/rust/api/client/device.dart';

class LiveInfo extends StatelessWidget {
  const LiveInfo({super.key, required this.device});

  final Device device;

  @override
  Widget build(BuildContext context) {
    return Container(
      child: StreamBuilder(
        stream: deviceSubscribeRealtimeStatus(device: device),
        builder: (context, snapshot) {
          return Text("Todo :)");
        },
      ),
    );
  }
}
