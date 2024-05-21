import 'package:flutter/material.dart';
import 'package:gui_flutter/src/app/devices/device_list.dart';
import 'package:gui_flutter/src/rust/api/client.dart';

class MainMenu extends StatefulWidget {
  const MainMenu({super.key, required this.client});

  final Client client;

  @override
  State<StatefulWidget> createState() {
    return _MainMenuState();
  }
}

class _MainMenuState extends State<MainMenu> {
  int _tabIndex = 0;

  late List<Widget> _tabs;

  @override
  void initState() {
    super.initState();

    _tabs = [DeviceList(client: widget.client), const Text("TODO")];
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text("Svalin"),
      ),
      body: _tabs[_tabIndex],
      bottomNavigationBar: BottomNavigationBar(
          currentIndex: _tabIndex,
          onTap: (value) => setState(() => _tabIndex = value),
          items: const [
            BottomNavigationBarItem(
              icon: Icon(Icons.computer),
              label: "Devices",
            ),
            BottomNavigationBarItem(
              icon: Icon(Icons.person),
              label: "Profile",
            ),
          ]),
    );
  }
}
