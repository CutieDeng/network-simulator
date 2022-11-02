from time import sleep
import mysocket

me = mysocket.MySocket() 

me.bind(("127.4.4.4", 4444))

time_cost = 1 
val = 0 

basic = b"see\n"

while True: 
    sleep(time_cost)
    print ("wait %f" % time_cost) 
    me.sendto(basic, ("127.4.5.6", 4444))
    print ("[%d] complete send " % val)
    val += 1 