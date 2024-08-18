import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';
import 'package:gui_flutter/src/app/devices/life_info/cpu.dart';
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
          if (snapshot.data == null) {
            return CircularProgressIndicator();
          } else {
            switch (snapshot.data!) {
              case RemoteLiveDataRealtimeStatus_Unavailable():
                return Text("Live data unavailable");
              case RemoteLiveDataRealtimeStatus_Pending():
                return CircularProgressIndicator();
              case RemoteLiveDataRealtimeStatus_Ready(
                  field0: final realtimeStatus
                ):
                return Column(
                  children: [
                    CpuDisplay(
                      cpuStatus: realtimeStatus.cpu,
                    )
                  ],
                );
            }
          }
          return Text("Todo :)");
        },
      ),
    );
  }
}
