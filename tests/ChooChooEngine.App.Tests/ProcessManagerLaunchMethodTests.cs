using System.Diagnostics;
using ChooChooEngine.App.Core;

namespace ChooChooEngine.App.Tests;

public sealed class ProcessManagerLaunchMethodTests
{
	[Theory]
	[InlineData(LaunchMethod.CreateThreadInjection, "CreateThreadInjection launch is not implemented")]
	[InlineData(LaunchMethod.RemoteThreadInjection, "RemoteThreadInjection launch is not implemented")]
	[InlineData((LaunchMethod)999, "Launch method '999' is not supported.")]
	public void GetUnsupportedLaunchMethodMessage_ReturnsExpectedMessage(LaunchMethod method, string expected)
	{
		string message = ProcessManager.GetUnsupportedLaunchMethodMessage(method);

		Assert.Contains(expected, message);
	}

	[Fact]
	public void TryRequireStartedProcess_ReturnsFalse_WhenProcessStartReturnsNull()
	{
		List<string> failures = new();

		bool result = ProcessManager.TryRequireStartedProcess(
			null,
			"Process.Start for shell execute launch",
			failures.Add,
			out Process startedProcess);

		Assert.False(result);
		Assert.Null(startedProcess);
		Assert.Single(failures);
		Assert.Contains("returned null", failures[0]);
	}

	[Fact]
	public void TryRequireStartedProcess_ReturnsTrue_WhenProcessExists()
	{
		using Process currentProcess = Process.GetCurrentProcess();
		List<string> failures = new();

		bool result = ProcessManager.TryRequireStartedProcess(
			currentProcess,
			"Process.Start for direct launch",
			failures.Add,
			out Process startedProcess);

		Assert.True(result);
		Assert.Same(currentProcess, startedProcess);
		Assert.Empty(failures);
	}
}
