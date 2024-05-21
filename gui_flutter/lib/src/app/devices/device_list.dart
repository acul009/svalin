import 'package:flutter/material.dart';
import 'package:gui_flutter/src/app/devices/add_device.dart';
import 'package:gui_flutter/src/rust/api/client.dart';

class DeviceList extends StatefulWidget {
  const DeviceList({super.key, required this.client});

  final Client client;

  @override
  State<StatefulWidget> createState() {
    return _DeviceListState();
  }
}

class _DeviceListState extends State<DeviceList> {
  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        Positioned(
          bottom: 20,
          right: 20,
          child: FloatingActionButton(
            onPressed: () {
              Navigator.push(
                  context,
                  MaterialPageRoute(
                    builder: (context) =>
                        AddDeviceDialog(client: widget.client),
                  ));
            },
            child: const Icon(Icons.add),
          ),
        ),
      ],
    );
  }
}
