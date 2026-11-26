"""Final integrity errors must prevent a successful result from being written."""
def checked_success(success,errors,verify):
 try:verify()
 except Exception as error:errors.append('Final input verification failed: '+str(error))
 return success and not errors
