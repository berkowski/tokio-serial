# Appveyor support files

From [Apollon77/SupportingFiles/README_SERIAL_TESTING.md](https://github.com/Apollon77/SupportingFiles/blob/master/README_SERIAL_TESTING.md),
preserved locally for use.  Original text below:

# Serial Port testing

## Serial testing on Appveyor for Windows
Because of the fact that Appveyor is container based they do not provide any serial ports by default.

One way to still do Serial testing on Appveyor is to use a "Virtual Serialport Driver"/"Null Modem Simulator" like com0com (http://com0com.sourceforge.net/). In here are all files needed for this.

Additionally you can also use com2tcp (also from http://com0com.sourceforge.net/)

### com0com Installer
Because com0com is a driver it needs to be installed and also the certificate needs to be allowed.

You need the following in your appveyor.yml:

```
install:
  - ps: Start-FileDownload https://github.com/Apollon77/SupportingFiles/raw/master/appveyor/serial/com0com.cer
  - ps: C:\"Program Files"\"Microsoft SDKs"\Windows\v7.1\Bin\CertMgr.exe /add com0com.cer /s /r localMachine root
  - ps: C:\"Program Files"\"Microsoft SDKs"\Windows\v7.1\Bin\CertMgr.exe /add com0com.cer /s /r localMachine trustedpublisher
  - ps: Start-FileDownload https://github.com/Apollon77/SupportingFiles/raw/master/appveyor/serial/setup_com0com_W7_x64_signed.exe
  - ps: $env:CNC_INSTALL_CNCA0_CNCB0_PORTS="YES"
  - ps: .\setup_com0com_W7_x64_signed.exe /S
  - ps: sleep 60
```

After that you will have a virtual serial port pair with the names CNCA0 and CNCB0 that are connected to each other that you can use for testing. Make sure to use  "\\.\CNCA0" and "\\.\CNCB0" to connect to them.

### com2tcp
To be able to create a Serialport-to-TCP tunnel you can use com2tcp, this can simplify testing too.

To get the program use the following in your appveyor.yml:

```
install:
  - ps: Start-FileDownload https://github.com/Apollon77/SupportingFiles/raw/master/appveyor/serial/com2tcp.exe
```

After that the com2tcp.exe is normally located in your %APPVEYOR_BUILD_FOLDER& which is the normal project clone folder.
Call it (in Background) using e.g.:

```
com2tcp.exe --ignore-dsr --baud 9600 --parity e \\.\CNCA0 127.0.0.1 15001
```
to connect the CNCA0 port to a TCP server on localhost on port 15001

### Credits
The final solution on how to use com0com on Appveyor was found by https://github.com/hybridgroup/rubyserial. I copied some file to my space to make sure they are available as I need them.

## Serial testing on Travis-CI for Linux and macOS
To simplify it you use socat here which is a standard tool.
To have it available for both Linux and macOS on Travis-CI add the following to your .travis.yml:

```
before_install:
  - 'if [[ "$TRAVIS_OS_NAME" == "osx" ]]; then brew update; fi'
  - 'if [[ "$TRAVIS_OS_NAME" == "osx" ]]; then brew install socat; fi'
addons:
  apt:
    packages:
      - socat
```

After this use socat to create a virtual serial port connected to a TCP server like:

```
socat -Dxs pty,link=/tmp/virtualcom0,ispeed=9600,ospeed=9600,raw tcp:127.0.0.1:15001
```

... or comparable to provide a virtual serialport pair.
