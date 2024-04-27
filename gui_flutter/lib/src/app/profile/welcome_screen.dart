import 'package:flutter/material.dart';
import 'package:gui_flutter/src/app/profile/new_profile.dart';
import 'package:gui_flutter/src/rust/api/simple.dart';

class ProfileSelector extends StatefulWidget {
  const ProfileSelector({super.key});

  @override
  State<ProfileSelector> createState() => _ProfileSelectorState();
}

class _ProfileSelectorState extends State<ProfileSelector> {
  late Future<List<String>> _profiles;
  // List<DropdownMenuEntry<String>> _profiles = [];
  String? _selectedProfile = null;

  @override
  void initState() {
    super.initState();
    _profiles = listProfiles();
    _profiles.then((value) => {
          if (value.isEmpty)
            {
              Navigator.pushReplacement(context,
                  MaterialPageRoute(builder: (context) => ServerDialog()))
            }
        });
    // listProfiles().then((value) => _profiles =
    //     value.map((e) => DropdownMenuEntry(value: e, label: e)).toList());
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: FutureBuilder(
          future: _profiles,
          builder: (context, snapshot) {
            if (snapshot.hasData) {
              var profiles = snapshot.data!;
              return Column(
                children: [
                  DropdownButton<String>(
                    items: profiles
                        .map((e) => DropdownMenuItem(value: e, child: Text(e)))
                        .toList(),
                    onChanged: (value) =>
                        setState(() => _selectedProfile = value),
                  )
                ],
              );
            } else {
              return const CircularProgressIndicator();
            }
          },
        ),
      ),
    );
  }
}
