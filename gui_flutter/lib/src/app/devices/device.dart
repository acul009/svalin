import 'package:flutter/material.dart';
import 'package:gui_flutter/src/app/devices/life_info/life_info.dart';
import 'package:gui_flutter/src/app/devices/terminal.dart';
import 'package:gui_flutter/src/rust/api/client.dart';

class DeviceView extends StatelessWidget {
  const DeviceView({super.key, required this.device});

  final Device device;

  @override
  Widget build(BuildContext context) {
    var itemFuture = device.item();

    return Scaffold(
      appBar: AppBar(
        title: FutureBuilder(
          future: itemFuture,
          builder: (context, snapshot) {
            if (snapshot.hasData) {
              return Row(
                children: [
                  Icon(
                    size: 50,
                    color:
                        snapshot.data!.onlineStatus ? Colors.green : Colors.red,
                    Icons.computer,
                  ),
                  const SizedBox(width: 20),
                  Text(snapshot.data!.publicData.name),
                ],
              );
            } else {
              return const CircularProgressIndicator();
            }
          },
        ),
      ),
      body: SingleChildScrollView(
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Column(
            children: [
              LiveInfo(device: device),
              const SizedBox(height: 20),
              ElevatedButton(
                  onPressed: () {
                    Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => TerminalWidget(device: device),
                        ));
                  },
                  child: const Text("Terminal"))
            ],
          ),
        ),
      ),
    );
  }
}
