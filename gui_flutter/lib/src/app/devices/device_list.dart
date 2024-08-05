import 'dart:async';

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
  List<AgentListItem> items = [];
  late Timer timer;

  void updateList() {
    widget.client.deviceList().then((value) => setState(() {
          items = value;
        }));
  }

  @override
  void initState() {
    super.initState();
    updateList();
    timer = Timer.periodic(
      const Duration(seconds: 1),
      (timer) => updateList(),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        ListView.builder(
          itemCount: items.length,
          itemBuilder: (context, index) {
            var item = items[index];
            return ListTile(
              title: Row(
                children: [
                  Icon(
                    size: 50,
                    color: item.onlineStatus ? Colors.green : Colors.red,
                    Icons.computer,
                  ),
                  const SizedBox(width: 20),
                  Text(item.publicData.name),
                ],
              ),
            );
          },
        ),
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
