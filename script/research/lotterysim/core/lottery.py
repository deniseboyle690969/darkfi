import matplotlib.pyplot as plt
from tqdm import tqdm
import time
from datetime import timedelta
from core.darkie import *
from pid.cascade import *
from tqdm import tqdm

class DarkfiTable:
    def __init__(self, airdrop, running_time, controller_type=CONTROLLER_TYPE_DISCRETE, kp=0, ki=0, kd=0, dt=1, kc=0, ti=0, td=0, ts=0, debug=False, r_kp=0, r_ki=0, r_kd=0):
        self.Sigma=airdrop
        self.darkies = []
        self.running_time=running_time
        self.start_time=None
        self.end_time=None
        self.secondary_pid = SecondaryDiscretePID(kp=kp, ki=ki, kd=kd) if controller_type==CONTROLLER_TYPE_DISCRETE else SecondaryTakahashiPID(kc=kc, ti=ti, td=td, ts=ts)
        print('secondary min/max : {}/{}'.format(self.secondary_pid.clip_min, self.secondary_pid.clip_max))
        self.primary_pid = PrimaryDiscretePID(kp=r_kp, ki=r_ki, kd=r_kd) if controller_type==CONTROLLER_TYPE_DISCRETE else PrimaryTakahashiPID(kc=kc, ti=ti, td=td, ts=ts)
        print('primary min/max : {}/{}'.format(self.primary_pid.clip_min, self.primary_pid.clip_max))
        self.debug=debug
        self.rewards = []
        self.winners = []

    def add_darkie(self, darkie):
        self.darkies+=[darkie]

    def background(self, rand_running_time=True, debug=False, hp=True):
        self.debug=debug
        self.start_time=time.time()
        feedback=0 # number leads in previous slot
        # random running time
        rand_running_time = random.randint(1,self.running_time) if rand_running_time else self.running_time
        self.running_time = rand_running_time
        #if rand_running_time and debug:
            #print("random running time: {}".format(self.running_time))
            #print('running time: {}'.format(self.running_time))

        rt_range = tqdm(np.arange(0,self.running_time, 1))
        for count in rt_range:
        #while count < self.running_time:
            winners=0
            f = self.secondary_pid.pid_clipped(float(feedback), debug)

            if count%EPOCH_LENGTH == 0:
                acc = self.secondary_pid.acc()
                #staked_ratio = self.avg_stake_ratio()
                reward = self.primary_pid.pid_clipped(acc, debug)
                self.rewards += [reward]


            #note! thread overhead is 10X slower than sequential node execution!
            total_stake = 0
            for i in range(len(self.darkies)):
                self.darkies[i].set_sigma_feedback(self.Sigma, feedback, f, count, hp)
                #self.darkies[i].update_vesting()
                self.darkies[i].run(hp)
                total_stake += self.darkies[i].stake
                #if self.darkies[i].stake>0:
                    #print('darkie {} has stake {} for slot {}'.format(i, self.darkies[i].stake, count))
            #print('reward: {}'.format(rewards[-1]))
            for i in range(len(self.darkies)):
                winners += self.darkies[i].won_hist[-1]
                ###
            self.winners +=[winners]
            feedback = winners
            if self.winners[-1]==1:
                for i in range(len(self.darkies)):
                    if self.darkies[i].won_hist[-1]:
                        self.darkies[i].update_stake(self.rewards[-1])
                        break
                # resolve finalization
                self.Sigma += self.rewards[-1]
                # resync nodes
                merge_length = 0
                for i in reversed(self.winners[:-1]):
                    if i !=1:
                        merge_length+=1
                    else:
                        break
                for i in range(merge_length):
                    resync_slot_id = count-(i+1)
                    resync_reward_id = int((resync_slot_id)/EPOCH_LENGTH)
                    resync_reward = self.rewards[resync_reward_id]
                    # resyncing depends on the random branch chosen,
                    # it's simulated by choosing first wining node
                    darkie_winning_idx = 0
                    random.shuffle(self.darkies)
                    for darkie_idx in range(len(self.darkies)):
                        if self.darkies[darkie_idx].won_hist[resync_slot_id]:
                            darkie_winning_idx = darkie_idx
                            break
                    self.darkies[darkie_winning_idx].resync_stake(resync_reward)
                    self.Sigma += resync_reward
            rt_range.set_description('issuance {} DRK, acc: {}, stake = {}%'.format(round(sum(self.rewards),2), round(acc,2), round(total_stake/self.Sigma*100 if self.Sigma>0 else 0,2)))
            #print('[2]stake: {}, sigma: {}, reward: {}'.format(total_stake, self.Sigma, self.rewards[-1]))
            assert(round(total_stake,1) <= round(self.Sigma,1))
            count+=1
        self.end_time=time.time()
        avg_reward = sum(self.rewards)/len(self.rewards)
        stake_ratio = self.avg_stake_ratio()
        avg_apy = self.avg_apy()
        avg_apr = self.avg_apr()
        #print('apy: {}, staked_ratio: {}'.format(avg_apy, stake_ratio))
        return self.secondary_pid.acc_percentage(), avg_apy, avg_reward, stake_ratio, avg_apr

    def avg_apy(self):
        return Num(sum([darkie.apy_scaled_to_runningtime(self.rewards) for darkie in self.darkies])/len(self.darkies))

    def avg_apr(self):
        return Num(sum([darkie.apr_scaled_to_runningtime() for darkie in self.darkies])/len(self.darkies))

    def avg_stake_ratio(self):
        return sum([darkie.staked_tokens_ratio() for darkie in self.darkies])/len(self.darkies)

    def write(self):
        elapsed=self.end_time-self.start_time
        for id, darkie in enumerate(self.darkies):
            darkie.write(id)
        if self.debug:
            print("total time: {}, slot time: {}".format(str(timedelta(seconds=elapsed)), str(timedelta(seconds=elapsed/self.running_time))))
        self.secondary_pid.write()
        with open('log/rewards.log', 'w+') as f:
            buff = ','.join([str(i) for i in self.rewards])
            f.write(buff)
