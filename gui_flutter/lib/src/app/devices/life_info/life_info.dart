import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';
import 'package:gui_flutter/src/app/devices/life_info/cpu.dart';
import 'package:gui_flutter/src/app/devices/life_info/memory.dart';
import 'package:gui_flutter/src/rust/api/client.dart';
import 'package:gui_flutter/src/rust/api/client/device.dart';

class LiveInfo extends StatelessWidget {
  const LiveInfo({super.key, required this.device});

  final Device device;

  Stream<RemoteLiveDataRealtimeStatus> _realtimeStream() async* {
    var receiver = await device.subscribeRealtime();
    while (true) {
      await receiver.changed();
      yield await receiver.currentOwned();
    }
  }

  @override
  Widget build(BuildContext context) {
    return StreamBuilder(
      stream: _realtimeStream(),
      builder: (context, snapshot) {
        if (snapshot.data == null || snapshot.data!.isPending()) {
          return const CircularProgressIndicator();
        } else {
          var realtime = snapshot.data!.getReady();
          if (realtime != null) {
            return Column(
              children: [
                CpuDisplay(
                  cpuStatus: realtime.cpu,
                ),
                MemoryDisplay(
                  memoryStatus: realtime.memory,
                  swapStatus: realtime.swap,
                ),
              ],
            );
          } else {
            return const Text("Realtime status not available at the moment");
          }
        }
      },
    );
  }
}
