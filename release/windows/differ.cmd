@rem Starts differ from cmd or PowerShell. --launcher lets differ.exe start its window in the background,
@rem so the command returns instead of waiting for differ to exit.
@"%~dp0..\differ.exe" --launcher %*
