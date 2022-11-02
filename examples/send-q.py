# In this part, I'd like to send to the same ip router as far as I can ... when time elapses 

from time import sleep
import mysocket 

me = mysocket.MySocket() 
me.bind(("127.6.6.6", 6666))

print ("sender binds successfully ")

time_wait = 5 
while True: 
    print (f"wait for {time_wait} s, then send a packet with 1 byte (80)")
    sleep(time_wait) 
    time_wait *= 0.95 
    if time_wait < 0.1 : 
        time_wait = 0.1 
    me.sendto(b"\x80", ("127.6.6.6", 6665)) 
