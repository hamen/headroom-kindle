#!/usr/bin/env python3
import paramiko
import os
import sys

# IP locale del Kindle via USBNetwork
KINDLE_IP = '192.168.15.244'
KINDLE_USER = 'root'
# Usiamo la password calcolata dal seriale come fallback, anche se l'SSH key è iniettata
KINDLE_PASSWORD = '***REMOVED***' 

# Path locali e remote
LOCAL_IMAGE = os.path.join(os.path.dirname(__file__), '../dashboard.png')
REMOTE_IMAGE = '/mnt/us/dashboard.png'

def push_and_draw():
    if not os.path.exists(LOCAL_IMAGE):
        print(f"Errore: Immagine {LOCAL_IMAGE} non trovata. Esegui 'cargo build && ./target/debug/headroom-kindle' prima.")
        sys.exit(1)

    client = paramiko.SSHClient()
    client.set_missing_host_key_policy(paramiko.AutoAddPolicy())

    try:
        print(f"Connecting to Kindle at {KINDLE_IP}...")
        # Prova con la chiave SSH locale, fallback su password
        client.connect(KINDLE_IP, username=KINDLE_USER, password=KINDLE_PASSWORD, timeout=10)
        
        print("Uploading dashboard.png...")
        sftp = client.open_sftp()
        sftp.put(LOCAL_IMAGE, REMOTE_IMAGE)
        sftp.close()
        
        print("Taking over the E-Ink display...")
        # Comando magico: uccidi la UI amazon (ignora errore se già morta), fai 2 flash neri per pulire lo schermo, disegna l'immagine
        command = 'stop lab126_gui 2>/dev/null; /usr/sbin/eips -c; /usr/sbin/eips -c; sleep 1; /usr/sbin/eips -f -g ' + REMOTE_IMAGE
        stdin, stdout, stderr = client.exec_command(command)
        
        # Aspettiamo che finisca
        stdout.read()
        err = stderr.read().decode('utf-8').strip()
        if err:
            print(f"Kindle stderr: {err}")
            
        print("Done! Check the Kindle screen.")

    except Exception as e:
        print(f"Connection failed: {e}")
        print("Assicurati che il Kindle sia connesso, che l'interfaccia usb0/enx... sia su, e che USBNetwork sia attivo (;un).")
    finally:
        client.close()

if __name__ == '__main__':
    push_and_draw()
